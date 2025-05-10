import { useState } from 'react';
import report from '../../../report.json';
import {FormattedTestResults} from '@jest/test-result';

type FormattedAssertionResult = FormattedTestResults[`testResults`][number][`assertionResults`][number];

function processTestResults(results: FormattedAssertionResult[]) {
  const groups: Record<string, FormattedAssertionResult[]> = {};
  
  for (const result of results) {
    const groupKey = result.ancestorTitles.join(' › ');
    if (!groups[groupKey]) {
      groups[groupKey] = [];
    }
    groups[groupKey].push(result);
  }
  
  return groups;
}

function Switch({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      onClick={onChange}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
        checked ? 'bg-green-500' : 'bg-gray-300'
      }`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
          checked ? 'translate-x-6' : 'translate-x-1'
        }`}
      />
    </button>
  );
}

function SearchIcon() {
  return (
    <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
    </svg>
  );
}

function CrossIcon() {
  return (
    <svg className="w-5 h-5 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}

function ClipboardIcon() {
  return (
    <svg className="w-5 h-5 text-gray-400 hover:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
    </svg>
  );
}

function ChevronIcon({ isExpanded }: { isExpanded: boolean }) {
  return (
    <svg 
      className={`w-5 h-5 text-gray-500 transform transition-transform ${isExpanded ? 'rotate-180' : ''}`} 
      fill="none" 
      stroke="currentColor" 
      viewBox="0 0 24 24"
    >
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
    </svg>
  );
}

function Card({ children, className = '' }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`bg-white rounded-lg shadow-lg p-6 ${className}`}>
      {children}
    </div>
  );
}

function SearchBar({ 
  value, 
  onChange,
  showSuccessful,
  onToggleSuccessful,
}: { 
  value: string;
  onChange: (value: string) => void;
  showSuccessful: boolean;
  onToggleSuccessful: () => void;
}) {
  return (
    <div className="flex items-center space-x-4">
      <div className="flex-1 flex items-center space-x-4">
        <SearchIcon />
        <input
          type="text"
          placeholder="Search tests..."
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="w-full px-4 py-2 rounded-lg bg-gray-50 border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>
      <div className="flex items-center space-x-3 text-sm text-gray-600">
        <span>Show successful tests</span>
        <Switch checked={showSuccessful} onChange={onToggleSuccessful} />
      </div>
    </div>
  );
}

function TestGrid({ results }: { results: FormattedAssertionResult[] }) {
  return (
    <Card className="mb-8">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(12px,1fr))] gap-0.5 max-w-[800px]">
        {results.map((test, index) => {
          const anchor = `${test.ancestorTitles.join('-')}-${test.title}`.toLowerCase().replace(/[^a-z0-9]+/g, '-');
          const statusClass = test.status === 'passed' ? 'bg-green-500' : 'bg-red-500';

          return (
            <a 
              key={index} 
              href={`#${anchor}`} 
              className={`relative aspect-square rounded hover:ring-2 hover:z-10 ring-offset-1 ring-blue-500 transition-all ${statusClass}`} 
              title={`${test.ancestorTitles.join(' › ')} › ${test.title}`}
            />
          );
        })}
      </div>
    </Card>
  )
}

interface TestLineProps {
  test: FormattedAssertionResult;
  onCopyToClipboard: () => void;
  children?: React.ReactNode;
}

function TestLine({ test, onCopyToClipboard, children }: TestLineProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const statusClass = test.status === 'passed' ? 'bg-green-50' : 'bg-red-50';
  const isExpandable = Boolean(children);

  return (
    <div className="rounded-lg">
      <div className="flex items-center gap-2">
        <button 
          className={`rounded px-2 h-10 leading-10 flex-1 flex items-center space-x-2 ${statusClass} hover:ring-2 ring-blue-500 ring-offset-2 cursor-pointer`} 
          onClick={() => isExpandable && setIsExpanded(!isExpanded)}
        >
          {test.status === 'passed' ? <CheckIcon /> : <CrossIcon />}
          <span className="flex-1 text-left">{test.title}</span>
          <span className="text-sm text-gray-500">{test.duration}ms</span>
          {isExpandable && <ChevronIcon isExpanded={isExpanded} />}
        </button>
        <button 
          onClick={onCopyToClipboard} 
          className="h-10 leading-10 aspect-square flex items-center justify-center bg-gray-50 hover:bg-gray-100 rounded transition-colors cursor-pointer" 
          title="Copy test name to clipboard"
        >
          <ClipboardIcon />
        </button>
      </div>
      {isExpanded && children}
    </div>
  );
}

interface TestFailureDetailsProps {
  failureMessages?: string[] | null;
}

function enhanceMessage(message: string) {
  message = message.replace(/\/[a-z0-9A-Z/_.-]+\/berry\//g, '/path/to/berry/');
  message = message.replace(/\/path\/to\/berry\/(packages\/[a-z0-9A-Z/_.-]+):([0-9]+):([0-9]+)/g, ($0, $1, $2) => `<a class="text-red-800 underline" href="https://github.com/yarnpkg/berry/blob/master/${$1}#L${$2}" target="_blank">${$0}</a>`);
  return message;
}

function TestFailureDetails({ failureMessages }: TestFailureDetailsProps) {
  if (!failureMessages?.length)
    return null;

  return (
    <div className="p-4 bg-white border-t border-gray-100">
      <div className="space-y-2 overflow-x-auto">
        {failureMessages.map((message, i) => (
          <pre key={i} className="whitespace-pre text-sm text-red-600 font-mono" dangerouslySetInnerHTML={{ __html: enhanceMessage(message) }} />
        ))}
      </div>
    </div>
  );
}

function TestGroups({ results }: { results: FormattedAssertionResult[] }) {
  const groups = processTestResults(results);

  const copyToClipboard = (test: FormattedAssertionResult) => {
    const escapedTitle = `${test.ancestorTitles.join(' ')} ${test.title}`
      .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      .replace(/'/g, "\\'");
    
    const shellSafeString = `'${escapedTitle}'`;
    navigator.clipboard.writeText(shellSafeString);
  };

  if (Object.keys(groups).length === 0) {
    return (
      <div className="text-center py-12 text-gray-500">
        No tests match your criteria
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {Object.entries(groups).map(([groupPath, tests]) => (
        <div key={groupPath} className="border-t border-gray-200 pt-6 first:border-t-0 first:pt-0">
          <h3 className="text-lg font-semibold mb-4 text-gray-700">
            {groupPath}
          </h3>

          <div className="space-y-2">
            {tests.map((test, index) => {
              const testId = `${test.ancestorTitles.join('-')}-${test.title}`.toLowerCase().replace(/[^a-z0-9]+/g, '-');
              const hasFailureDetails = test.status !== 'passed';

              return (
                <div key={index} id={testId}>
                  <TestLine
                    test={test}
                    onCopyToClipboard={() => copyToClipboard(test)}
                  >
                    {hasFailureDetails && (
                      <TestFailureDetails failureMessages={test.failureMessages} />
                    )}
                  </TestLine>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

export default function App() {
  const typedReport = report as any as FormattedTestResults;

  console.log(typedReport);

  const [showSuccessful, setShowSuccessful] = useState(true);
  const [search, setSearch] = useState('');
  
  const allResults = typedReport.testResults.map(result => result.assertionResults).flat();
  const filteredResults = allResults.filter(result => {
    const matchesFilter = showSuccessful || result.status !== 'passed';

    const searchTerm = search.toLowerCase();
    const matchesSearch = 
      search === '' ||
      result.title.toLowerCase().includes(searchTerm) ||
      result.ancestorTitles.some(title => title.toLowerCase().includes(searchTerm));

    return matchesFilter && matchesSearch;
  });

  return (
    <div className="bg-gray-100 min-h-screen p-8">
      <div className="max-w-7xl mx-auto">
        <TestGrid results={allResults} />
        <Card>
          <div className="mb-6">
            <SearchBar 
              value={search} 
              onChange={setSearch} 
              showSuccessful={showSuccessful} 
              onToggleSuccessful={() => setShowSuccessful(!showSuccessful)}
            />
          </div>
          <TestGroups results={filteredResults} />
        </Card>
      </div>
    </div>
  );
} 
